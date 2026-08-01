op babelforce-get-settings-for-telephony-agent-inbound -> Any
  description "Get agent.inbound settings"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/telephony/agent.inbound")
  response = http.request(method: "GET", url)
  return response
