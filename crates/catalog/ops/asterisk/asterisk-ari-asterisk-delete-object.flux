op asterisk-ari-asterisk-delete-object(configClass: String, objectType: String, id: String) -> Any
  description "Delete a dynamic configuration object."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/config/dynamic/{configClass}/{objectType}/{id}")
  response = http.request(method: "DELETE", url)
  return response
