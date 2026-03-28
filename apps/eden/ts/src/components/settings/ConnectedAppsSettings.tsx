import { useCallback, useEffect, useState } from 'react'

interface ConnectedAppsSettingsProps {
  onRefreshData: () => Promise<void>;
}

interface HevyStatus {
  loggedIn: boolean
  username: string | null
}

interface SyncStats {
  workoutsCreated: number
  workoutsSkipped: number
  exerciseEntriesCreated: number
}

export default function ConnectedAppsSettings({ onRefreshData }: ConnectedAppsSettingsProps) {
  const [hevyStatus, setHevyStatus] = useState<HevyStatus>({ loggedIn: false, username: null })
  const [loginError, setLoginError] = useState<string | null>(null)
  const [loginLoading, setLoginLoading] = useState(false)
  const [syncing, setSyncing] = useState(false)
  const [syncResult, setSyncResult] = useState<SyncStats | null>(null)
  const [syncError, setSyncError] = useState<string | null>(null)

  const checkStatus = useCallback(async () => {
    if (!window.api?.hevyGetAuthStatus) return
    const status = await window.api.hevyGetAuthStatus()
    setHevyStatus(status)
  }, [])

  useEffect(() => {
    void checkStatus()
  }, [checkStatus])

  const handleLogin = async () => {
    if (!window.api?.hevyLogin) return
    setLoginLoading(true)
    setLoginError(null)
    const result = await window.api.hevyLogin()
    setLoginLoading(false)
    if (result.ok) {
      void checkStatus()
    } else {
      setLoginError(result.error)
    }
  }

  const handleLogout = async () => {
    if (!window.api?.hevyLogout) return
    await window.api.hevyLogout()
    setHevyStatus({ loggedIn: false, username: null })
    setSyncResult(null)
  }

  const handleSync = async () => {
    if (!window.api?.hevySyncWorkouts) return
    setSyncing(true)
    setSyncError(null)
    setSyncResult(null)
    const result = await window.api.hevySyncWorkouts()
    setSyncing(false)
    if (result.ok) {
      setSyncResult(result.stats)
      await onRefreshData()
    } else {
      setSyncError(result.error)
    }
  }

  return (
    <div className="settings-section" data-testid="connected-apps-settings">
      <h2>Связанные программы</h2>
      <p className="settings-description">Импорт данных из внешних приложений в Eden.</p>

      <div className="connected-app-card" data-testid="hevy-card">
        <div className="connected-app-header">
          <div className="connected-app-icon">
            <img
              src="/anytype/icon/type/default/barbell.svg"
              alt="Hevy"
              width={24}
              height={24}
              draggable={false}
            />
          </div>
          <div className="connected-app-info">
            <h3>Hevy</h3>
            <p>Трекер тренировок — импорт тренировок и упражнений</p>
          </div>
          <div className="connected-app-badge">
            {hevyStatus.loggedIn ? (
              <span className="badge badge-connected" data-testid="hevy-status-connected">Подключено</span>
            ) : (
              <span className="badge badge-disconnected" data-testid="hevy-status-disconnected">Не подключено</span>
            )}
          </div>
        </div>

        {hevyStatus.loggedIn ? (
          <div className="connected-app-body">
            <p data-testid="hevy-username">Аккаунт: <strong>{hevyStatus.username || '—'}</strong></p>

            <div className="connected-app-actions">
              <button
                className="settings-btn-primary"
                onClick={() => { void handleSync() }}
                disabled={syncing}
                data-testid="hevy-sync-btn"
                type="button"
              >
                {syncing ? 'Синхронизация...' : 'Синхронизировать тренировки'}
              </button>
              <button
                className="settings-btn-secondary"
                onClick={() => { void handleLogout() }}
                data-testid="hevy-logout-btn"
                type="button"
              >
                Отключить
              </button>
            </div>

            {syncResult && (
              <div className="sync-result" data-testid="hevy-sync-result">
                <p>Добавлено тренировок: <strong>{syncResult.workoutsCreated}</strong></p>
                <p>Пропущено (уже есть): <strong>{syncResult.workoutsSkipped}</strong></p>
                <p>Добавлено упражнений: <strong>{syncResult.exerciseEntriesCreated}</strong></p>
              </div>
            )}

            {syncError && (
              <div className="dialog-error" data-testid="hevy-sync-error">{syncError}</div>
            )}
          </div>
        ) : (
          <div className="connected-app-body">
            <p>Войдите в аккаунт Hevy через браузер. Откроется окно hevy.com, где вы сможете авторизоваться.</p>
            <button
              className="settings-btn-primary"
              onClick={() => { void handleLogin() }}
              disabled={loginLoading}
              data-testid="hevy-login-btn"
              type="button"
            >
              {loginLoading ? 'Открываю окно...' : 'Войти через Hevy'}
            </button>
            {loginError && (
              <div className="dialog-error" data-testid="hevy-login-error">{loginError}</div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
