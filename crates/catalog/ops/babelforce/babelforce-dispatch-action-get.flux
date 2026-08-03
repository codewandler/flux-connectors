op babelforce-dispatch-action-get(integrationId: String, action: String, callId: String, sessionId: String) -> Any
  description "Run an integration action (GET)"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{integrationId}/dispatch/{action}")
  response = http.request(method: "GET", query: { callId, sessionId }, url)
  return response
