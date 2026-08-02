op asterisk-ari-applications-get(applicationName: String) -> Any
  description "Get details of an application."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/applications/{applicationName}")
  response = http.request(method: "GET", url)
  return response
