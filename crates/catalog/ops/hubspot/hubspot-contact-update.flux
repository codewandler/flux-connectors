op hubspot-contact-update(contact_id: Number, firstname: String, lastname: String) -> Any
  description "Overwrite a contact's first and last name. Both are written on every call — this operation replaces them rather than merging, so re-send the one you are not changing (read it first with hubspot-contact-get). The change is visible to everyone in the portal and can trigger a workflow. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/category` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://api.hubapi.com"
  $url = fmt("{base}/crm/v3/objects/contacts/{contact_id}")
  $content_type = "application/json"
  $payload = { properties: { firstname: $firstname, lastname: $lastname } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "PATCH", url: $url })
  return $response
