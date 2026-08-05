op front-conversation-get(conversation_id: String) -> Any
  description "Get one conversation's metadata: subject, status, assignee, recipient and tags. Does not return its messages — use front-conversation-message-list for those. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/_error/message`, its error code at `/_error/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api2.frontapp.com"
  url = fmt("{base}/conversations/{conversation_id}")
  response = http.request(method: "GET", url)
  return response
