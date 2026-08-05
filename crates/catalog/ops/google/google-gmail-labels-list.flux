op google-gmail-labels-list(user_id: String) -> Any
  description "List every label in a mailbox — the system labels (`INBOX`, `SENT`, `SPAM`) and the user's own — as `{\"labels\": [...]}`. Needs the `gmail.readonly` scope (or `gmail.labels`). A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://gmail.googleapis.com"
  url = fmt("{base}/gmail/v1/users/{user_id}/labels")
  response = http.request(method: "GET", url)
  return response
