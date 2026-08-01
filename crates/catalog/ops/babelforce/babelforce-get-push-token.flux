op babelforce-get-push-token -> Any
  description "Get the current user's push token"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/push-token")
  response = http.request(method: "GET", url)
  return response
