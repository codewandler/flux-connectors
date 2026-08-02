op asterisk-ari-mailboxes-delete(mailboxName: String) -> Any
  description "Destroy a mailbox."
  risk "destructive"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/mailboxes/{mailboxName}")
  response = http.request(method: "DELETE", url)
  return response
