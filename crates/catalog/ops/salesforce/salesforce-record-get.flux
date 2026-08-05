op salesforce-record-get(sobject_type: String, id: String) -> Any
  description "Read one SObject record by id. Returns every field of the record; narrowing to a subset needs the fields query parameter, which this connector cannot encode safely (see the provider file's header) so it is not offered"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{instance}.my.salesforce.com"
  url = fmt("{base}/services/data/v59.0/sobjects/{sobject_type}/{id}")
  response = http.request(method: "GET", url)
  return response
