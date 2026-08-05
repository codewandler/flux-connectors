op asterisk-ari-mailboxes-list -> Any
  description "List all mailboxes."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/mailboxes")
  response = http.request(method: "GET", url)
  return response
