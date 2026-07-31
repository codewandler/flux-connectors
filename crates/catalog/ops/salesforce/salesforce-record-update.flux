op salesforce-record-update(sobject_type: String, id: String, body: Any) -> Any
  description "Update one or more fields on an existing SObject record. Only the supplied fields change; fields left out are untouched. Answers 204 with no body on success"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{instance}.my.salesforce.com"
  url = fmt("{base}/services/data/v59.0/sobjects/{sobject_type}/{id}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response
