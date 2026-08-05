op asterisk-ari-asterisk-get-global-var(variable: String) -> Any
  description "Get the value of a global variable."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/variable")
  response = http.request(method: "GET", query: { variable }, url)
  return response
