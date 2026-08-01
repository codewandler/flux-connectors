op babelforce-list-single-action-session-variables(provider: String, actionName: String) -> Any
  description "List an action's session variables"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/actions/{provider}/{actionName}/variables")
  response = http.request(method: "GET", url)
  return response
