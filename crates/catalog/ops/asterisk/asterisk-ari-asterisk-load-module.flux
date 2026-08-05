op asterisk-ari-asterisk-load-module(moduleName: String) -> Any
  description "Load an Asterisk module."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/modules/{moduleName}")
  response = http.request(method: "POST", url)
  return response
