op babelforce-list-action-params(providerName: String, providerActionName: String) -> Any
  description "List an action's parameters"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/actions/{providerName}/{providerActionName}/params")
  response = http.request(method: "GET", url)
  return response
