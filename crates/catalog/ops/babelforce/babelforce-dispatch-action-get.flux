op babelforce-dispatch-action-get(integrationId: String, action: String, callId: String, sessionId: String) -> Any
  description "Run an integration action (GET)"
  risk "low"
  idempotency "idempotent"
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
  response = http.request(method: "GET", url)
  return response
