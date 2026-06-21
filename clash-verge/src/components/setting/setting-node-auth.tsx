import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { DialogRef } from '@/components/base'
import { nodeAuthGetStatus } from '@/services/cmds'

import { NodeAuthViewer } from './mods/node-auth-viewer'
import { SettingItem, SettingList } from './mods/setting-comp'

interface Props {
  onError?: (err: Error) => void
}

const SettingNodeAuth = ({ onError }: Props) => {
  const { t } = useTranslation()
  const viewerRef = useRef<DialogRef>(null)
  const [status, setStatus] = useState<INodeAuthStatus | null>(null)

  useEffect(() => {
    nodeAuthGetStatus()
      .then(setStatus)
      .catch((err) => onError?.(err as Error))
  }, [onError])

  const statusText = !status?.logged_in
    ? t('settings.sections.nodeAuth.notLoggedIn')
    : status.expired
      ? `${status.username} (${t('settings.sections.nodeAuth.expired')})`
      : status.username

  return (
    <SettingList title={t('settings.sections.nodeAuth.title')}>
      <NodeAuthViewer ref={viewerRef} onChanged={setStatus} />

      <SettingItem
        label={t('settings.sections.nodeAuth.status')}
        secondary={statusText}
        onClick={() => viewerRef.current?.open()}
      />
    </SettingList>
  )
}

export default SettingNodeAuth
