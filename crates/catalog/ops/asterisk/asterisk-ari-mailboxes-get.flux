op asterisk-ari-mailboxes-get(mailboxName: String) -> Any
  description "Retrieve the current state of a mailbox."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/mailboxes/{mailboxName}")
  response = http.request(method: "GET", url)
  return response
