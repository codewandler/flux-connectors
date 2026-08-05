op babelforce-get-settings-for-telephony-agent-wrapup -> Any
  description "Get agent.wrapup settings"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/telephony/agent.wrapup")
  response = http.request(method: "GET", url)
  return response
