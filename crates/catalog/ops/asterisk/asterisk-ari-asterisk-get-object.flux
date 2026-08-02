op asterisk-ari-asterisk-get-object(configClass: String, objectType: String, id: String) -> Any
  description "Retrieve a dynamic configuration object."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/config/dynamic/{configClass}/{objectType}/{id}")
  response = http.request(method: "GET", url)
  return response
