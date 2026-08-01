op babelforce-dispatch-action(integrationId: String, action: String, callId: String, sessionId: String, body: Any) -> Any
  description "Run an integration action"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{integrationId}/dispatch/{action}")
  sep = "?"
  when callId
    url = fmt("{url}{sep}callId={callId}")
    sep = "&"
  when sessionId
    url = fmt("{url}{sep}sessionId={sessionId}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
