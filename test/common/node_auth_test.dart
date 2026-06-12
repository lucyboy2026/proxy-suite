import 'package:fl_clash/common/node_auth.dart';
import 'package:fl_clash/models/node_auth.dart';
import 'package:fl_clash/providers/action.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Seed a persisted node-auth session into mocked SharedPreferences.
Future<void> _seedSession({
  required String token,
  required DateTime expiresAt,
  String password = 'pw',
}) async {
  final session = NodeAuthSession.fromLoginJson(
    'https://auth.example.com',
    {'token': token, 'expires_at': expiresAt.toIso8601String()},
    password: password,
  );
  SharedPreferences.setMockInitialValues({
    'node_auth_session': session.encode(),
  });
}

Map<String, dynamic> _sampleConfig() => {
  'proxies': [
    {'name': 'a', 'type': 'hysteria2', 'password': 'placeholder'},
    {'name': 'b', 'type': 'hysteria', 'password': 'placeholder'},
    {'name': 'c', 'type': 'ss', 'password': 'keep-me'},
  ],
};

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('NodeAuth.currentToken', () {
    test('returns the token while valid', () async {
      await _seedSession(
        token: 'live-token',
        expiresAt: DateTime.now().add(const Duration(days: 5)),
      );
      expect(await nodeAuth.currentToken(), 'live-token');
    });

    test('returns null once expired', () async {
      await _seedSession(
        token: 'old-token',
        expiresAt: DateTime.now().subtract(const Duration(minutes: 1)),
      );
      expect(await nodeAuth.currentToken(), isNull);
    });

    test('returns null when logged out', () async {
      SharedPreferences.setMockInitialValues({});
      expect(await nodeAuth.currentToken(), isNull);
    });
  });

  group('injectNodeAuthToken', () {
    test('overwrites password on hysteria2/hysteria nodes only', () async {
      await _seedSession(
        token: 'device-token',
        expiresAt: DateTime.now().add(const Duration(days: 5)),
      );
      final config = _sampleConfig();
      await injectNodeAuthToken(config);
      final proxies = config['proxies'] as List;
      expect(proxies[0]['password'], 'device-token');
      expect(proxies[1]['password'], 'device-token');
      expect(proxies[2]['password'], 'keep-me');
    });

    test('leaves config untouched when logged out', () async {
      SharedPreferences.setMockInitialValues({});
      final config = _sampleConfig();
      await injectNodeAuthToken(config);
      final proxies = config['proxies'] as List;
      expect(proxies[0]['password'], 'placeholder');
      expect(proxies[1]['password'], 'placeholder');
    });

    test('leaves config untouched when token expired', () async {
      await _seedSession(
        token: 'old-token',
        expiresAt: DateTime.now().subtract(const Duration(minutes: 1)),
      );
      final config = _sampleConfig();
      await injectNodeAuthToken(config);
      expect((config['proxies'] as List)[0]['password'], 'placeholder');
    });

    test('tolerates a config without proxies', () async {
      await _seedSession(
        token: 'device-token',
        expiresAt: DateTime.now().add(const Duration(days: 5)),
      );
      final config = <String, dynamic>{'rules': <String>[]};
      await injectNodeAuthToken(config);
      expect(config.containsKey('proxies'), isFalse);
    });
  });
}
