op asterisk-ari-asterisk-get-global-var(variable: String) -> Any
  description "Get the value of a global variable."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/variable?variable={variable}")
  response = http.request(method: "GET", url)
  return response
