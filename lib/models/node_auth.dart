import 'dart:convert';

/// Persisted login state for the device-bound node-auth feature.
///
/// Mirrors the Auth Server `LoginResponse` contract:
/// `{ token, expires_at, username, max_devices, active_devices,
///    account_expires_at, subscription_url }`.
class NodeAuthSession {
  final String serverUrl;
  final String email;
  final String token;
  final DateTime? tokenExpiresAt;
  final DateTime? accountExpiresAt;
  final int maxDevices;
  final int activeDevices;
  final String subscriptionUrl;

  const NodeAuthSession({
    required this.serverUrl,
    required this.email,
    required this.token,
    required this.tokenExpiresAt,
    required this.accountExpiresAt,
    required this.maxDevices,
    required this.activeDevices,
    required this.subscriptionUrl,
  });

  factory NodeAuthSession.fromLoginJson(
    String serverUrl,
    Map<String, dynamic> json,
  ) {
    return NodeAuthSession(
      serverUrl: serverUrl,
      email: (json['username'] as String?) ?? '',
      token: (json['token'] as String?) ?? '',
      tokenExpiresAt: _parseDate(json['expires_at']),
      accountExpiresAt: _parseDate(json['account_expires_at']),
      maxDevices: _parseInt(json['max_devices']),
      activeDevices: _parseInt(json['active_devices']),
      subscriptionUrl: (json['subscription_url'] as String?) ?? '',
    );
  }

  Map<String, dynamic> toJson() => {
    'serverUrl': serverUrl,
    'email': email,
    'token': token,
    'tokenExpiresAt': tokenExpiresAt?.toIso8601String(),
    'accountExpiresAt': accountExpiresAt?.toIso8601String(),
    'maxDevices': maxDevices,
    'activeDevices': activeDevices,
    'subscriptionUrl': subscriptionUrl,
  };

  factory NodeAuthSession.fromJson(Map<String, dynamic> json) {
    return NodeAuthSession(
      serverUrl: (json['serverUrl'] as String?) ?? '',
      email: (json['email'] as String?) ?? '',
      token: (json['token'] as String?) ?? '',
      tokenExpiresAt: _parseDate(json['tokenExpiresAt']),
      accountExpiresAt: _parseDate(json['accountExpiresAt']),
      maxDevices: _parseInt(json['maxDevices']),
      activeDevices: _parseInt(json['activeDevices']),
      subscriptionUrl: (json['subscriptionUrl'] as String?) ?? '',
    );
  }

  String encode() => jsonEncode(toJson());

  static NodeAuthSession? decode(String? raw) {
    if (raw == null || raw.isEmpty) return null;
    try {
      return NodeAuthSession.fromJson(
        jsonDecode(raw) as Map<String, dynamic>,
      );
    } catch (_) {
      return null;
    }
  }

  static DateTime? _parseDate(Object? value) {
    if (value is String && value.isNotEmpty) {
      return DateTime.tryParse(value)?.toLocal();
    }
    return null;
  }

  static int _parseInt(Object? value) {
    if (value is int) return value;
    if (value is num) return value.toInt();
    if (value is String) return int.tryParse(value) ?? 0;
    return 0;
  }
}

/// Result of a `/register` call. The server returns 202 with
/// `{ status, message }`; `status` is one of `pending`/`active`/`suspended`.
class NodeAuthRegisterResult {
  final String status;
  final String message;

  const NodeAuthRegisterResult({required this.status, required this.message});

  factory NodeAuthRegisterResult.fromJson(Map<String, dynamic> json) {
    return NodeAuthRegisterResult(
      status: (json['status'] as String?) ?? 'pending',
      message: (json['message'] as String?) ?? '',
    );
  }
}
