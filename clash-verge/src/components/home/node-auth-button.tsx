import { AccountCircleOutlined } from '@mui/icons-material'
import { Button, Tooltip } from '@mui/material'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { DialogRef } from '@/components/base'
import { NodeAuthViewer } from '@/components/setting/mods/node-auth-viewer'
import { nodeAuthGetStatus } from '@/services/cmds'

/// 首页右上角的「节点账号」入口：显示登录状态，点击打开登录/注册弹窗。
export const NodeAuthButton = () => {
  const { t } = useTranslation()
  const viewerRef = useRef<DialogRef>(null)
  const [status, setStatus] = useState<INodeAuthStatus | null>(null)

  useEffect(() => {
    nodeAuthGetStatus()
      .then(setStatus)
      .catch((err) => console.error('[NodeAuthButton] status', err))
  }, [])

  const label = !status?.logged_in
    ? t('settings.sections.nodeAuth.notLoggedIn')
    : status.expired
      ? `${status.username} (${t('settings.sections.nodeAuth.expired')})`
      : status.username

  return (
    <>
      <NodeAuthViewer ref={viewerRef} onChanged={setStatus} />
      <Tooltip title={t('settings.sections.nodeAuth.title')} arrow>
        <Button
          size="small"
          color={status?.logged_in && !status.expired ? 'inherit' : 'warning'}
          startIcon={<AccountCircleOutlined />}
          onClick={() => viewerRef.current?.open()}
          sx={{ textTransform: 'none', maxWidth: 220, mr: 0.5 }}
        >
          {label}
        </Button>
      </Tooltip>
    </>
  )
}
