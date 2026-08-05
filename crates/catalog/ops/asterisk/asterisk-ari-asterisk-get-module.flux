op asterisk-ari-asterisk-get-module(moduleName: String) -> Any
  description "Get Asterisk module information."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/modules/{moduleName}")
  response = http.request(method: "GET", url)
  return response
