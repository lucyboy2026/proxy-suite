import 'dart:convert';

/// Persisted login state for the device-bound node-auth feature.
///
/// Mirrors the Auth Server `LoginResponse` contract:
/// `{ token, expires_at, username, max_devices, active_devices,
///    account_expires_at, subscription_url }`.
class NodeAuthSession {
  final String serverUrl;
  final String email;

  /// The email the user actually typed at login, kept locally to drive silent
  /// renewal. [email] is derived from the server's `username` field, which is
  /// not guaranteed to equal the login email, so renewal uses this instead.
  final String loginEmail;
  final String token;
  final DateTime? tokenExpiresAt;
  final DateTime? accountExpiresAt;
  final int maxDevices;
  final int activeDevices;
  final String subscriptionUrl;

  /// Account password kept locally so the token can be renewed silently
  /// (parity with the Clash Verge client, which stores it for `renew_if_needed`).
  /// Never shown in the UI and never logged.
  final String password;

  const NodeAuthSession({
    required this.serverUrl,
    required this.email,
    this.loginEmail = '',
    required this.token,
    required this.tokenExpiresAt,
    required this.accountExpiresAt,
    required this.maxDevices,
    required this.activeDevices,
    required this.subscriptionUrl,
    this.password = '',
  });

  factory NodeAuthSession.fromLoginJson(
    String serverUrl,
    Map<String, dynamic> json, {
    String password = '',
    String loginEmail = '',
  }) {
    return NodeAuthSession(
      serverUrl: serverUrl,
      email: (json['username'] as String?) ?? '',
      loginEmail: loginEmail,
      token: (json['token'] as String?) ?? '',
      tokenExpiresAt: _parseDate(json['expires_at']),
      accountExpiresAt: _parseDate(json['account_expires_at']),
      maxDevices: _parseInt(json['max_devices']),
      activeDevices: _parseInt(json['active_devices']),
      subscriptionUrl: (json['subscription_url'] as String?) ?? '',
      password: password,
    );
  }

  Map<String, dynamic> toJson() => {
    'serverUrl': serverUrl,
    'email': email,
    'loginEmail': loginEmail,
    'token': token,
    'tokenExpiresAt': tokenExpiresAt?.toIso8601String(),
    'accountExpiresAt': accountExpiresAt?.toIso8601String(),
    'maxDevices': maxDevices,
    'activeDevices': activeDevices,
    'subscriptionUrl': subscriptionUrl,
    'password': password,
  };

  factory NodeAuthSession.fromJson(Map<String, dynamic> json) {
    return NodeAuthSession(
      serverUrl: (json['serverUrl'] as String?) ?? '',
      email: (json['email'] as String?) ?? '',
      loginEmail: (json['loginEmail'] as String?) ?? '',
      token: (json['token'] as String?) ?? '',
      tokenExpiresAt: _parseDate(json['tokenExpiresAt']),
      accountExpiresAt: _parseDate(json['accountExpiresAt']),
      maxDevices: _parseInt(json['maxDevices']),
      activeDevices: _parseInt(json['activeDevices']),
      subscriptionUrl: (json['subscriptionUrl'] as String?) ?? '',
      password: (json['password'] as String?) ?? '',
    );
  }

  /// Whether the device token has passed its expiry. Mirrors the Clash Verge
  /// client's `expired` flag. Returns `false` when the expiry is unknown so a
  /// usable token is never treated as expired by mistake.
  bool get isTokenExpired {
    final exp = tokenExpiresAt;
    if (exp == null) return false;
    return DateTime.now().isAfter(exp);
  }

  /// Whether the account itself has reached its validity deadline.
  bool get isAccountExpired {
    final exp = accountExpiresAt;
    if (exp == null) return false;
    return DateTime.now().isAfter(exp);
  }

  /// Whether the token is within [renewWindow] of expiry and should be renewed
  /// silently. Requires a stored password to be actionable.
  bool needsRenewal(Duration renewWindow) {
    final exp = tokenExpiresAt;
    if (exp == null) return false;
    if (password.isEmpty) return false;
    return DateTime.now().isAfter(exp.subtract(renewWindow));
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
