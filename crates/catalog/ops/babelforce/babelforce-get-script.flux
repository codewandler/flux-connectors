op babelforce-get-script(codeId: String, type: String, response_2: String) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/scripts/{type}/{codeId}")
  response = http.request(method: "GET", query: { response: response_2 }, url)
  return response
