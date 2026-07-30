op intercom-contact-create(role: String, email: String) -> Any
  description "Create an email-identified Intercom contact. A contact created here is visible to every teammate in the workspace and is counted against its contact quota; creating one twice creates two contacts unless the workspace deduplicates on email. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://api.intercom.io"
  $url = fmt("{base}/contacts")
  $content_type = "application/json"
  $payload = { email: $email, role: $role }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
