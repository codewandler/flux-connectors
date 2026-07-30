op freshdesk-ticket-note-add(id: Number, body: String, private: Bool, incoming: Bool, notify_emails: List<String>) -> Any
  description "Add a note to a ticket; the note is private unless explicitly made public"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://example.freshdesk.com/api/v2"
  $url = fmt("{base}/tickets/{id}/notes")
  $content_type = "application/json"
  $payload = { body: $body, incoming: $incoming, notify_emails: $notify_emails, private: $private }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
