import { useEffect, useRef } from 'react'

import { DialogRef } from '@/components/base'
import { NodeAuthViewer } from '@/components/setting/mods/node-auth-viewer'
import { nodeAuthGetStatus, nodeAuthRenew } from '@/services/cmds'

/** 续期检查间隔：12 小时 */
const RENEW_INTERVAL_MS = 12 * 60 * 60 * 1000

/**
 * 启动鉴权门：
 * - 启动时静默续期一次，并在未登录/已过期时自动弹出登录框
 * - 每 12h 触发一次「临近过期则续期」
 */
export function NodeAuthGate() {
  const viewerRef = useRef<DialogRef>(null)

  useEffect(() => {
    let cancelled = false

    const checkOnStartup = async () => {
      try {
        await nodeAuthRenew().catch(() => false)
        const status = await nodeAuthGetStatus()
        if (cancelled) return
        if (!status.logged_in || status.expired) {
          viewerRef.current?.open()
        }
      } catch (err) {
        console.error('[NodeAuthGate] startup check failed', err)
      }
    }

    checkOnStartup()

    const timer = window.setInterval(() => {
      nodeAuthRenew().catch((err) =>
        console.error('[NodeAuthGate] renew failed', err),
      )
    }, RENEW_INTERVAL_MS)

    return () => {
      cancelled = true
      window.clearInterval(timer)
    }
  }, [])

  return <NodeAuthViewer ref={viewerRef} />
}
