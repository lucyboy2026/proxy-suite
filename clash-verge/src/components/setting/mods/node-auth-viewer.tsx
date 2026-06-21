import {
  Box,
  CircularProgress,
  List,
  ListItem,
  ListItemText,
  TextField,
  Typography,
} from '@mui/material'
import { useLockFn } from 'ahooks'
import { useImperativeHandle, useState, type Ref } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseDialog, DialogRef } from '@/components/base'
import {
  nodeAuthGetDeviceFp,
  nodeAuthGetStatus,
  nodeAuthLogin,
  nodeAuthLogout,
} from '@/services/cmds'
import { showNotice } from '@/services/notice-service'

interface Props {
  ref?: Ref<DialogRef>
  onChanged?: (status: INodeAuthStatus) => void
}

export function NodeAuthViewer({ ref, onChanged }: Props) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [isWorking, setIsWorking] = useState(false)

  const [status, setStatus] = useState<INodeAuthStatus | null>(null)
  const [deviceFp, setDeviceFp] = useState('')
  const [server, setServer] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')

  const refresh = async () => {
    const [st, fp] = await Promise.all([
      nodeAuthGetStatus(),
      nodeAuthGetDeviceFp(),
    ])
    setStatus(st)
    setDeviceFp(fp)
    setServer(st.server || '')
    setUsername(st.username || '')
    return st
  }

  useImperativeHandle(ref, () => ({
    open: () => {
      setOpen(true)
      setPassword('')
      refresh().catch((err) => console.error('[NodeAuthViewer] refresh', err))
    },
    close: () => setOpen(false),
  }))

  const onLogin = useLockFn(async () => {
    if (!server.trim()) {
      showNotice.error('settings.sections.nodeAuth.messages.serverRequired')
      return
    }
    if (!username.trim()) {
      showNotice.error('settings.sections.nodeAuth.messages.usernameRequired')
      return
    }
    if (!password) {
      showNotice.error('settings.sections.nodeAuth.messages.passwordRequired')
      return
    }
    try {
      setIsWorking(true)
      const st = await nodeAuthLogin(server.trim(), username.trim(), password)
      setStatus(st)
      setPassword('')
      onChanged?.(st)
      showNotice.success('settings.sections.nodeAuth.messages.loginSuccess')
      setOpen(false)
    } catch (err) {
      showNotice.error(
        'settings.sections.nodeAuth.messages.loginFailed',
        err,
        4000,
      )
    } finally {
      setIsWorking(false)
    }
  })

  const onLogout = useLockFn(async () => {
    try {
      setIsWorking(true)
      await nodeAuthLogout()
      const st = await refresh()
      onChanged?.(st)
      showNotice.success('settings.sections.nodeAuth.messages.logoutSuccess')
    } catch (err) {
      showNotice.error(
        'shared.feedback.notifications.common.saveFailed',
        err,
        4000,
      )
    } finally {
      setIsWorking(false)
    }
  })

  const loggedIn = !!status?.logged_in

  return (
    <BaseDialog
      open={open}
      title={t('settings.sections.nodeAuth.title')}
      contentSx={{ width: 420 }}
      okBtn={
        isWorking ? (
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <CircularProgress size={16} color="inherit" />
            {t('shared.statuses.saving')}
          </Box>
        ) : loggedIn ? (
          t('settings.sections.nodeAuth.actions.relogin')
        ) : (
          t('settings.sections.nodeAuth.actions.login')
        )
      }
      cancelBtn={t('shared.actions.cancel')}
      disableOk={isWorking}
      onClose={() => setOpen(false)}
      onCancel={() => setOpen(false)}
      onOk={onLogin}
    >
      <Typography variant="body2" color="text.secondary" sx={{ mb: 1.5 }}>
        {t('settings.sections.nodeAuth.description')}
      </Typography>

      <List sx={{ py: 0 }}>
        <ListItem sx={{ px: 0, py: 0.5 }}>
          <ListItemText
            primary={t('settings.sections.nodeAuth.deviceFp')}
            secondary={
              <Box component="span" sx={{ wordBreak: 'break-all' }}>
                {deviceFp || '-'}
              </Box>
            }
          />
        </ListItem>
        {loggedIn && (
          <>
            <ListItem sx={{ px: 0, py: 0.5 }}>
              <ListItemText
                primary={t('settings.sections.nodeAuth.expiresAt')}
                secondary={
                  status?.expired
                    ? `${status?.expires_at || '-'} (${t('settings.sections.nodeAuth.expired')})`
                    : status?.expires_at || '-'
                }
              />
            </ListItem>
            <ListItem sx={{ px: 0, py: 0.5 }}>
              <ListItemText
                primary={t('settings.sections.nodeAuth.devices')}
                secondary={`${status?.active_devices ?? '-'} / ${status?.max_devices ?? '-'}`}
              />
            </ListItem>
          </>
        )}
      </List>

      <TextField
        fullWidth
        size="small"
        sx={{ mt: 1 }}
        label={t('settings.sections.nodeAuth.fields.server')}
        placeholder={t('settings.sections.nodeAuth.placeholders.server')}
        value={server}
        onChange={(e) => setServer(e.target.value)}
        disabled={isWorking}
      />
      <TextField
        fullWidth
        size="small"
        sx={{ mt: 1.5 }}
        label={t('settings.sections.nodeAuth.fields.username')}
        placeholder={t('settings.sections.nodeAuth.placeholders.username')}
        value={username}
        onChange={(e) => setUsername(e.target.value)}
        disabled={isWorking}
      />
      <TextField
        fullWidth
        size="small"
        type="password"
        sx={{ mt: 1.5 }}
        label={t('settings.sections.nodeAuth.fields.password')}
        placeholder={t('settings.sections.nodeAuth.placeholders.password')}
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        disabled={isWorking}
      />

      {loggedIn && (
        <Box sx={{ mt: 2, display: 'flex', justifyContent: 'flex-end' }}>
          <Typography
            component="button"
            variant="body2"
            color="error"
            onClick={onLogout}
            sx={{
              background: 'none',
              border: 'none',
              cursor: isWorking ? 'default' : 'pointer',
              padding: 0,
            }}
          >
            {t('settings.sections.nodeAuth.actions.logout')}
          </Typography>
        </Box>
      )}
    </BaseDialog>
  )
}
