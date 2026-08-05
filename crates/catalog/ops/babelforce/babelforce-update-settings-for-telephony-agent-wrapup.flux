op babelforce-update-settings-for-telephony-agent-wrapup(body: Any) -> Any
  description "Update agent.wrapup settings"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/telephony/agent.wrapup")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
