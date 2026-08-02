op asterisk-ari-asterisk-reload-module(moduleName: String) -> Any
  description "Reload an Asterisk module."
  risk "high"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/modules/{moduleName}")
  response = http.request(method: "PUT", url)
  return response
