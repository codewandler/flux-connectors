op asterisk-ari-mailboxes-update(mailboxName: String, oldMessages: Number, newMessages: Number) -> Any
  description "Change the state of a mailbox. (Note - implicitly creates the mailbox)."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/mailboxes/{mailboxName}")
  response = http.request(method: "PUT", query: { newMessages, oldMessages }, url)
  return response
