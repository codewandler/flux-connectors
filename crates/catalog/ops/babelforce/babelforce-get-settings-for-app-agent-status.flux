op babelforce-get-settings-for-app-agent-status -> Any
  description "Get agent.status settings"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/app/agent.status")
  response = http.request(method: "GET", url)
  return response
