op asterisk-ari-asterisk-unload-module(moduleName: String) -> Any
  description "Unload an Asterisk module."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/modules/{moduleName}")
  response = http.request(method: "DELETE", url)
  return response
