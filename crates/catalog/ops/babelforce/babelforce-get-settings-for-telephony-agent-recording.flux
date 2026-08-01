op babelforce-get-settings-for-telephony-agent-recording -> Any
  description "Get agent.recording settings"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/telephony/agent.recording")
  response = http.request(method: "GET", url)
  return response
