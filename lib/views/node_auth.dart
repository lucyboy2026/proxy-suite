import 'package:fl_clash/common/common.dart';
import 'package:fl_clash/common/node_auth.dart';
import 'package:fl_clash/enum/enum.dart';
import 'package:fl_clash/models/models.dart';
import 'package:fl_clash/models/node_auth.dart';
import 'package:fl_clash/providers/providers.dart';
import 'package:fl_clash/state.dart';
import 'package:fl_clash/widgets/scaffold.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Returns the Chinese string on a zh locale, otherwise the English string.
/// Keeps this fork's new screens self-contained without touching the
/// generated l10n (`flutter_intl`) pipeline.
String _t(BuildContext context, String zh, String en) {
  return Localizations.localeOf(context).languageCode == 'zh' ? zh : en;
}

class NodeAuthView extends ConsumerStatefulWidget {
  const NodeAuthView({super.key});

  @override
  ConsumerState<NodeAuthView> createState() => _NodeAuthViewState();
}

class _NodeAuthViewState extends ConsumerState<NodeAuthView> {
  final _serverController = TextEditingController();
  final _emailController = TextEditingController();
  final _passwordController = TextEditingController();

  bool _isRegister = false;
  bool _busy = false;
  bool _obscure = true;
  NodeAuthSession? _session;
  String _deviceFp = '';

  @override
  void initState() {
    super.initState();
    _restore();
    _loadDeviceFp();
  }

  Future<void> _restore() async {
    final session = await nodeAuth.loadSession();
    if (!mounted) return;
    setState(() {
      _session = session;
      if (session != null) {
        _serverController.text = session.serverUrl;
        _emailController.text = session.email;
      }
    });
  }

  Future<void> _loadDeviceFp() async {
    final fp = await nodeAuth.deviceFingerprint();
    if (!mounted) return;
    setState(() => _deviceFp = fp);
  }

  @override
  void dispose() {
    _serverController.dispose();
    _emailController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  String? _validateInputs({required bool requirePassword}) {
    final server = _serverController.text.trim();
    final email = _emailController.text.trim();
    final password = _passwordController.text;
    if (server.isEmpty) {
      return _t(context, '请填写服务器地址', 'Server address is required');
    }
    if (!email.contains('@')) {
      return _t(context, '邮箱格式不正确', 'Invalid email format');
    }
    if (requirePassword && password.length < 6) {
      return _t(context, '密码至少 6 位', 'Password must be at least 6 characters');
    }
    return null;
  }

  void _snack(String message) {
    if (!mounted) return;
    context.showSnackBar(message);
  }

  Future<void> _handleRegister() async {
    final error = _validateInputs(requirePassword: true);
    if (error != null) {
      _snack(error);
      return;
    }
    setState(() => _busy = true);
    try {
      final result = await nodeAuth.register(
        serverUrl: _serverController.text,
        email: _emailController.text,
        password: _passwordController.text,
      );
      if (!mounted) return;
      await globalState.showMessage(
        title: _t(context, '注册', 'Register'),
        message: TextSpan(
          text: result.message.isNotEmpty
              ? result.message
              : _t(context, '注册成功，已通知管理员审核',
                  'Registered. Administrator has been notified.'),
        ),
        cancelable: false,
      );
      if (mounted) setState(() => _isRegister = false);
    } catch (e) {
      if (mounted) _snack(e.toString());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _handleLogin() async {
    final error = _validateInputs(requirePassword: true);
    if (error != null) {
      _snack(error);
      return;
    }
    setState(() => _busy = true);
    try {
      final session = await nodeAuth.login(
        serverUrl: _serverController.text,
        email: _emailController.text,
        password: _passwordController.text,
      );
      if (!mounted) return;
      setState(() {
        _session = session;
        _passwordController.clear();
      });
      _snack(_t(context, '登录成功', 'Logged in'));
      await _importSubscription(session);
    } catch (e) {
      if (mounted) _snack(e.toString());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  /// After login, import (or refresh) the user's fixed subscription URL.
  Future<void> _importSubscription(NodeAuthSession session) async {
    if (session.subscriptionUrl.isEmpty) return;
    try {
      await globalState.loadingRun(
        tag: LoadingTag.profiles,
        () async {
          final profilesNotifier = ref.read(profilesProvider.notifier);
          Profile? existing;
          for (final profile in ref.read(profilesProvider)) {
            if (profile.url == session.subscriptionUrl) {
              existing = profile;
              break;
            }
          }
          final base = existing ??
              Profile.normal(
                label: 'NodeAuth',
                url: session.subscriptionUrl,
              );
          final updated = await base.update();
          profilesNotifier.put(updated);
          if (ref.read(currentProfileIdProvider) == null) {
            ref.read(currentProfileIdProvider.notifier).value = updated.id;
          }
        },
        title: _t(context, '导入订阅', 'Import subscription'),
      );
      if (mounted) {
        _snack(_t(context, '订阅已导入', 'Subscription imported'));
      }
    } catch (e) {
      if (mounted) {
        final prefix = _t(
          context,
          '订阅导入失败: ',
          'Subscription import failed: ',
        );
        _snack('$prefix$e');
      }
    }
  }

  Future<void> _handleLogout() async {
    final confirm = await globalState.showMessage(
      title: _t(context, '退出登录', 'Sign out'),
      message: TextSpan(
        text: _t(context, '确定退出当前账号吗？', 'Sign out of the current account?'),
      ),
    );
    if (confirm != true) return;
    await nodeAuth.clearSession();
    if (!mounted) return;
    setState(() {
      _session = null;
      _passwordController.clear();
    });
  }

  @override
  Widget build(BuildContext context) {
    return CommonScaffold(
      title: _t(context, '节点账号', 'Node Account'),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (_session != null) ...[
            _StatusCard(
              session: _session!,
              deviceFp: _deviceFp,
              onRefresh: _busy ? null : () => _importSubscription(_session!),
              onLogout: _busy ? null : _handleLogout,
            ),
            const SizedBox(height: 16),
          ],
          _buildAuthForm(context),
        ],
      ),
    );
  }

  Widget _buildAuthForm(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            SegmentedButton<bool>(
              segments: [
                ButtonSegment(
                  value: false,
                  label: Text(_t(context, '登录', 'Login')),
                  icon: const Icon(Icons.login),
                ),
                ButtonSegment(
                  value: true,
                  label: Text(_t(context, '注册', 'Register')),
                  icon: const Icon(Icons.person_add_alt),
                ),
              ],
              selected: {_isRegister},
              onSelectionChanged: _busy
                  ? null
                  : (value) => setState(() => _isRegister = value.first),
            ),
            const SizedBox(height: 16),
            _DeviceFpRow(deviceFp: _deviceFp),
            const SizedBox(height: 12),
            TextField(
              controller: _serverController,
              enabled: !_busy,
              keyboardType: TextInputType.url,
              decoration: InputDecoration(
                labelText: _t(context, '服务器地址', 'Server address'),
                hintText: 'https://your-auth-server',
                border: const OutlineInputBorder(),
                prefixIcon: const Icon(Icons.dns_outlined),
              ),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _emailController,
              enabled: !_busy,
              keyboardType: TextInputType.emailAddress,
              decoration: InputDecoration(
                labelText: _t(context, '邮箱', 'Email'),
                hintText: 'you@example.com',
                border: const OutlineInputBorder(),
                prefixIcon: const Icon(Icons.email_outlined),
              ),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _passwordController,
              enabled: !_busy,
              obscureText: _obscure,
              decoration: InputDecoration(
                labelText: _t(context, '密码', 'Password'),
                border: const OutlineInputBorder(),
                prefixIcon: const Icon(Icons.lock_outline),
                suffixIcon: IconButton(
                  icon: Icon(
                    _obscure ? Icons.visibility_off : Icons.visibility,
                  ),
                  onPressed: () => setState(() => _obscure = !_obscure),
                ),
              ),
            ),
            const SizedBox(height: 20),
            FilledButton.icon(
              onPressed: _busy
                  ? null
                  : (_isRegister ? _handleRegister : _handleLogin),
              icon: _busy
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : Icon(_isRegister ? Icons.person_add_alt : Icons.login),
              label: Text(
                _isRegister
                    ? _t(context, '注册', 'Register')
                    : _t(context, '登录', 'Login'),
              ),
            ),
            const SizedBox(height: 8),
            Text(
              _isRegister
                  ? _t(
                      context,
                      '注册将采集本机设备指纹并提交，等待管理员授权后邮件通知。',
                      'Registration submits this device fingerprint; you will be '
                          'emailed once an administrator authorizes it.',
                    )
                  : _t(
                      context,
                      '登录后将展示使用期限与设备数，并自动拉取订阅。',
                      'After login, account validity and device count are shown '
                          'and the subscription is imported automatically.',
                    ),
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      ),
    );
  }
}

class _StatusCard extends StatelessWidget {
  final NodeAuthSession session;
  final String deviceFp;
  final VoidCallback? onRefresh;
  final VoidCallback? onLogout;

  const _StatusCard({
    required this.session,
    required this.deviceFp,
    required this.onRefresh,
    required this.onLogout,
  });

  String _formatDate(BuildContext context, DateTime? date) {
    if (date == null) return _t(context, '永久 / 未设置', 'Permanent / unset');
    final local = date.toLocal();
    String two(int v) => v.toString().padLeft(2, '0');
    return '${local.year}-${two(local.month)}-${two(local.day)} '
        '${two(local.hour)}:${two(local.minute)}';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card(
      color: theme.colorScheme.surfaceContainerHighest,
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(Icons.verified_user, color: theme.colorScheme.primary),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    session.email,
                    style: theme.textTheme.titleMedium,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ),
            const Divider(height: 24),
            _row(
              context,
              Icons.event_available,
              _t(context, '账号到期', 'Account expires'),
              _formatDate(context, session.accountExpiresAt),
              expired: session.isAccountExpired,
            ),
            _row(
              context,
              Icons.vpn_key_outlined,
              _t(context, 'Token 到期', 'Token expires'),
              _formatDate(context, session.tokenExpiresAt),
              expired: session.isTokenExpired,
            ),
            _row(
              context,
              Icons.devices,
              _t(context, '设备数', 'Devices'),
              '${session.activeDevices}/${session.maxDevices}',
            ),
            _row(
              context,
              Icons.fingerprint,
              _t(context, '设备指纹', 'Device FP'),
              deviceFp.isEmpty ? '-' : deviceFp,
            ),
            _row(
              context,
              Icons.link,
              _t(context, '订阅链接', 'Subscription URL'),
              session.subscriptionUrl.isEmpty ? '-' : session.subscriptionUrl,
            ),
            const SizedBox(height: 12),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                TextButton.icon(
                  onPressed: onRefresh,
                  icon: const Icon(Icons.refresh),
                  label: Text(_t(context, '刷新订阅', 'Refresh subscription')),
                ),
                const SizedBox(width: 8),
                TextButton.icon(
                  onPressed: onLogout,
                  icon: const Icon(Icons.logout),
                  label: Text(_t(context, '退出登录', 'Sign out')),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _row(
    BuildContext context,
    IconData icon,
    String label,
    String value, {
    bool expired = false,
  }) {
    final theme = Theme.of(context);
    final valueColor = expired ? theme.colorScheme.error : null;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            icon,
            size: 18,
            color: expired ? theme.colorScheme.error : theme.colorScheme.outline,
          ),
          const SizedBox(width: 10),
          SizedBox(
            width: 96,
            child: Text(label, style: theme.textTheme.bodyMedium),
          ),
          Expanded(
            child: Text(
              expired
                  ? '$value (${_t(context, '已过期', 'expired')})'
                  : value,
              style: theme.textTheme.bodyMedium?.copyWith(
                fontWeight: FontWeight.w500,
                color: valueColor,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Read-only display of the current device fingerprint, shown in the auth form
/// so users can see the identity that will be bound on register/login
/// (parity with the Clash Verge client's device-fp display).
class _DeviceFpRow extends StatelessWidget {
  final String deviceFp;

  const _DeviceFpRow({required this.deviceFp});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(Icons.fingerprint, size: 18, color: theme.colorScheme.outline),
        const SizedBox(width: 10),
        Text(
          _t(context, '设备指纹', 'Device FP'),
          style: theme.textTheme.bodySmall,
        ),
        const SizedBox(width: 10),
        Expanded(
          child: Text(
            deviceFp.isEmpty ? '...' : deviceFp,
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ),
      ],
    );
  }
}
