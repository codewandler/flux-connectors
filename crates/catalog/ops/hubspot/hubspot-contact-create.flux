op hubspot-contact-create(email: String) -> Any
  description "Create a contact from an email address. HubSpot treats email as the contact's unique identifier and rejects a duplicate with 409. The new record is visible to everyone in the portal and can be enrolled by a workflow, which may send it marketing email. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/category` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://api.hubapi.com"
  $url = fmt("{base}/crm/v3/objects/contacts")
  $content_type = "application/json"
  $payload = { properties: { email: $email } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
