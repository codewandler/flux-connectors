op docusign-envelope-void(envelope_id: String, voidedReason: String) -> Any
  description "Void an envelope: cancel it permanently. Every recipient who has not yet finished signing is immediately locked out, and no further signing action is possible on this envelope id. Irreversible. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/errorCode` in the response body."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{account_host}/restapi/v2.1/accounts/{account_id}"
  url = fmt("{base}/envelopes/{envelope_id}")
  content_type = "application/json"
  status = "voided"
  payload = { status, voidedReason }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
