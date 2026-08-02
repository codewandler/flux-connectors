op asterisk-ari-mailboxes-update(mailboxName: String, oldMessages: Number, newMessages: Number) -> Any
  description "Change the state of a mailbox. (Note - implicitly creates the mailbox)."
  risk "high"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/mailboxes/{mailboxName}?oldMessages={oldMessages}&newMessages={newMessages}")
  response = http.request(method: "PUT", url)
  return response
