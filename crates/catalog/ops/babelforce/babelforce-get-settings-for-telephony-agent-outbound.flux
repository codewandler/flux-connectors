op babelforce-get-settings-for-telephony-agent-outbound -> Any
  description "Get agent.outbound settings"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/telephony/agent.outbound")
  response = http.request(method: "GET", url)
  return response
