op babelforce-get-script(codeId: String, type: String, response_2: String) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/scripts/{type}/{codeId}")
  sep = "?"
  when response_2
    url = fmt("{url}{sep}response={response_2}")
  response = http.request(method: "GET", url)
  return response
