import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:crypto/crypto.dart';
import 'package:device_info_plus/device_info_plus.dart';
import 'package:dio/dio.dart';
import 'package:fl_clash/models/node_auth.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Thrown when an Auth Server request fails. [message] carries the server's
/// human-readable reason (the server returns the message as plain text for
/// non-2xx responses).
class NodeAuthException implements Exception {
  final String message;

  const NodeAuthException(this.message);

  @override
  String toString() => message;
}

/// Client for the device-bound two-step auth Auth Server.
///
/// Endpoints (see `server/src/routes/client.rs`):
/// - `POST /register` `{email, password, device_fp, platform}`
/// - `POST /login`    `{email, password, device_fp, platform}` -> session
class NodeAuth {
  static const _sessionKey = 'node_auth_session';
  static const _installIdKey = 'node_auth_install_id';

  static NodeAuth? _instance;

  final Dio _dio = Dio(
    BaseOptions(
      connectTimeout: const Duration(seconds: 15),
      receiveTimeout: const Duration(seconds: 30),
      // Accept any status so we can read the server's plain-text error body.
      validateStatus: (_) => true,
    ),
  );

  NodeAuth._internal();

  factory NodeAuth() {
    _instance ??= NodeAuth._internal();
    return _instance!;
  }

  String get platform => Platform.operatingSystem;

  /// Normalize a user-entered server address: trim, drop trailing slashes and
  /// default to `https://` when no scheme is given (parity with the Clash Verge
  /// client's `normalize_server`).
  String _normalizeServerUrl(String serverUrl) {
    var url = serverUrl.trim();
    while (url.endsWith('/')) {
      url = url.substring(0, url.length - 1);
    }
    if (url.isEmpty) return url;
    if (!url.startsWith('http://') && !url.startsWith('https://')) {
      url = 'https://$url';
    }
    return url;
  }

  /// A stable per-install device fingerprint: SHA-256 over hardware identity
  /// plus a random install id persisted on first launch. Stays constant across
  /// app restarts but is unique per device/install.
  Future<String> deviceFingerprint() async {
    final prefs = await SharedPreferences.getInstance();
    var installId = prefs.getString(_installIdKey);
    if (installId == null || installId.isEmpty) {
      final rng = Random.secure();
      final bytes = List<int>.generate(16, (_) => rng.nextInt(256));
      installId = base64Url.encode(bytes);
      await prefs.setString(_installIdKey, installId);
    }
    final hardware = await _hardwareSignature();
    final digest = sha256.convert(utf8.encode('$installId|$hardware'));
    return digest.toString();
  }

  Future<String> _hardwareSignature() async {
    final info = DeviceInfoPlugin();
    try {
      if (Platform.isAndroid) {
        final a = await info.androidInfo;
        return [
          a.brand,
          a.manufacturer,
          a.model,
          a.device,
          a.hardware,
          a.id,
        ].join('/');
      }
      if (Platform.isIOS) {
        final i = await info.iosInfo;
        return [i.name, i.model, i.identifierForVendor].join('/');
      }
      if (Platform.isWindows) {
        final w = await info.windowsInfo;
        return [w.computerName, w.deviceId, w.productId].join('/');
      }
      if (Platform.isMacOS) {
        final m = await info.macOsInfo;
        return [m.computerName, m.model, m.systemGUID].join('/');
      }
      if (Platform.isLinux) {
        final l = await info.linuxInfo;
        return [l.name, l.machineId, l.id].join('/');
      }
    } catch (_) {
      // Fall through to the platform name on any platform-info failure.
    }
    return Platform.operatingSystem;
  }

  Future<NodeAuthRegisterResult> register({
    required String serverUrl,
    required String email,
    required String password,
  }) async {
    final url = _normalizeServerUrl(serverUrl);
    final fp = await deviceFingerprint();
    final response = await _post('$url/register', {
      'email': email.trim(),
      'password': password,
      'device_fp': fp,
      'platform': platform,
    });
    final data = _decodeJson(response);
    if (data == null) {
      throw const NodeAuthException('注册响应解析失败 / invalid register response');
    }
    return NodeAuthRegisterResult.fromJson(data);
  }

  Future<NodeAuthSession> login({
    required String serverUrl,
    required String email,
    required String password,
  }) async {
    final url = _normalizeServerUrl(serverUrl);
    final fp = await deviceFingerprint();
    final response = await _post('$url/login', {
      'email': email.trim(),
      'password': password,
      'device_fp': fp,
      'platform': platform,
    });
    final data = _decodeJson(response);
    if (data == null) {
      throw const NodeAuthException('登录响应解析失败 / invalid login response');
    }
    final session = NodeAuthSession.fromLoginJson(url, data);
    await saveSession(session);
    return session;
  }

  Future<Response<dynamic>> _post(String url, Map<String, dynamic> body) async {
    final Response<dynamic> response;
    try {
      response = await _dio.post<dynamic>(
        url,
        data: body,
        options: Options(
          contentType: Headers.jsonContentType,
          responseType: ResponseType.plain,
        ),
      );
    } on DioException catch (e) {
      throw NodeAuthException(
        '无法连接服务器 / cannot reach server: ${e.message ?? e.type.name}',
      );
    }
    final code = response.statusCode ?? 0;
    if (code < 200 || code >= 300) {
      throw NodeAuthException(_errorMessage(response));
    }
    return response;
  }

  /// The server returns its error reason as plain text for non-2xx responses.
  String _errorMessage(Response<dynamic> response) {
    final raw = response.data;
    if (raw is String && raw.trim().isNotEmpty) {
      return raw.trim();
    }
    return '请求失败 / request failed (HTTP ${response.statusCode})';
  }

  Map<String, dynamic>? _decodeJson(Response<dynamic> response) {
    final raw = response.data;
    if (raw is Map<String, dynamic>) return raw;
    if (raw is String && raw.trim().isNotEmpty) {
      try {
        final decoded = jsonDecode(raw);
        if (decoded is Map<String, dynamic>) return decoded;
      } catch (_) {
        return null;
      }
    }
    return null;
  }

  Future<NodeAuthSession?> loadSession() async {
    final prefs = await SharedPreferences.getInstance();
    return NodeAuthSession.decode(prefs.getString(_sessionKey));
  }

  Future<void> saveSession(NodeAuthSession session) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_sessionKey, session.encode());
  }

  Future<void> clearSession() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_sessionKey);
  }
}

final nodeAuth = NodeAuth();
