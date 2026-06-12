import 'dart:convert';

import 'package:fl_clash/models/node_auth.dart';
import 'package:test/test.dart';

void main() {
  group('NodeAuthSession.fromLoginJson', () {
    test('maps the Auth Server LoginResponse contract', () {
      final session = NodeAuthSession.fromLoginJson('https://auth.example.com', {
        'token': 'abc123',
        'expires_at': '2999-01-01T00:00:00Z',
        'username': 'user@example.com',
        'max_devices': 3,
        'active_devices': 1,
        'account_expires_at': '2999-06-01T00:00:00Z',
        'subscription_url': 'https://auth.example.com/sub/key',
      });
      expect(session.serverUrl, 'https://auth.example.com');
      expect(session.email, 'user@example.com');
      expect(session.token, 'abc123');
      expect(session.maxDevices, 3);
      expect(session.activeDevices, 1);
      expect(session.subscriptionUrl, 'https://auth.example.com/sub/key');
      expect(session.tokenExpiresAt, isNotNull);
      expect(session.accountExpiresAt, isNotNull);
    });

    test('tolerates missing optional fields', () {
      final session = NodeAuthSession.fromLoginJson('https://a', {
        'token': 't',
      });
      expect(session.email, '');
      expect(session.maxDevices, 0);
      expect(session.activeDevices, 0);
      expect(session.tokenExpiresAt, isNull);
      expect(session.accountExpiresAt, isNull);
      expect(session.subscriptionUrl, '');
    });
  });

  group('NodeAuthSession JSON round-trip', () {
    test('encode/decode preserves all fields', () {
      final original = NodeAuthSession.fromLoginJson('https://auth', {
        'token': 'tok',
        'expires_at': '2999-01-01T00:00:00Z',
        'username': 'u@e.com',
        'max_devices': 5,
        'active_devices': 2,
        'account_expires_at': '2999-02-01T00:00:00Z',
        'subscription_url': 'https://auth/sub/k',
      });
      final restored = NodeAuthSession.decode(original.encode());
      expect(restored, isNotNull);
      expect(restored!.serverUrl, original.serverUrl);
      expect(restored.email, original.email);
      expect(restored.token, original.token);
      expect(restored.maxDevices, original.maxDevices);
      expect(restored.activeDevices, original.activeDevices);
      expect(restored.subscriptionUrl, original.subscriptionUrl);
      expect(
        restored.tokenExpiresAt?.toUtc(),
        original.tokenExpiresAt?.toUtc(),
      );
      expect(
        restored.accountExpiresAt?.toUtc(),
        original.accountExpiresAt?.toUtc(),
      );
    });

    test('decode returns null for empty/garbage input', () {
      expect(NodeAuthSession.decode(null), isNull);
      expect(NodeAuthSession.decode(''), isNull);
      expect(NodeAuthSession.decode('not-json'), isNull);
    });
  });

  group('expiry helpers', () {
    NodeAuthSession sessionWith({String? token, String? account}) {
      return NodeAuthSession.fromLoginJson('https://a', {
        'token': 't',
        'expires_at': ?token,
        'account_expires_at': ?account,
      });
    }

    test('isTokenExpired is false when expiry is unknown', () {
      expect(sessionWith().isTokenExpired, isFalse);
      expect(sessionWith().isAccountExpired, isFalse);
    });

    test('isTokenExpired is true for a past token expiry', () {
      expect(sessionWith(token: '2000-01-01T00:00:00Z').isTokenExpired, isTrue);
    });

    test('isTokenExpired is false for a future token expiry', () {
      expect(sessionWith(token: '2999-01-01T00:00:00Z').isTokenExpired, isFalse);
    });

    test('isAccountExpired tracks the account deadline', () {
      expect(
        sessionWith(account: '2000-01-01T00:00:00Z').isAccountExpired,
        isTrue,
      );
      expect(
        sessionWith(account: '2999-01-01T00:00:00Z').isAccountExpired,
        isFalse,
      );
    });
  });

  group('needsRenewal', () {
    NodeAuthSession sessionWith({String? token, String password = 'pw'}) {
      return NodeAuthSession.fromLoginJson(
        'https://a',
        {'token': 't', 'expires_at': ?token},
        password: password,
      );
    }

    const window = Duration(days: 2);

    test('is false without a stored password', () {
      final soon = DateTime.now().add(const Duration(hours: 1));
      expect(
        sessionWith(token: soon.toIso8601String(), password: '')
            .needsRenewal(window),
        isFalse,
      );
    });

    test('is false when expiry is unknown', () {
      expect(sessionWith().needsRenewal(window), isFalse);
    });

    test('is true within the renewal window', () {
      final soon = DateTime.now().add(const Duration(hours: 1));
      expect(
        sessionWith(token: soon.toIso8601String()).needsRenewal(window),
        isTrue,
      );
    });

    test('is false when comfortably before the window', () {
      final later = DateTime.now().add(const Duration(days: 5));
      expect(
        sessionWith(token: later.toIso8601String()).needsRenewal(window),
        isFalse,
      );
    });
  });

  group('password persistence', () {
    test('password survives encode/decode but is sourced explicitly', () {
      final session = NodeAuthSession.fromLoginJson(
        'https://a',
        {'token': 't'},
        password: 'secret',
      );
      expect(session.password, 'secret');
      final restored = NodeAuthSession.decode(session.encode());
      expect(restored!.password, 'secret');
    });

    test('fromLoginJson defaults password to empty', () {
      final session = NodeAuthSession.fromLoginJson('https://a', {'token': 't'});
      expect(session.password, '');
    });
  });

  group('NodeAuthRegisterResult', () {
    test('defaults status to pending and message to empty', () {
      final result = NodeAuthRegisterResult.fromJson(<String, dynamic>{});
      expect(result.status, 'pending');
      expect(result.message, '');
    });

    test('parses the server payload', () {
      final result = NodeAuthRegisterResult.fromJson(
        jsonDecode('{"status":"active","message":"ok"}') as Map<String, dynamic>,
      );
      expect(result.status, 'active');
      expect(result.message, 'ok');
    });
  });
}
