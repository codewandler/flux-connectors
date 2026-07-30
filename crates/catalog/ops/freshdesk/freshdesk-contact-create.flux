op freshdesk-contact-create(name: String, email: String, phone: String) -> Any
  description "Create a contact"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://{domain}/api/v2"
  $url = fmt("{base}/contacts")
  $content_type = "application/json"
  $payload = { email: $email, name: $name, phone: $phone }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
