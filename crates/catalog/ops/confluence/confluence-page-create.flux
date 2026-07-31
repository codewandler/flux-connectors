op confluence-page-create(space_id: String, title: String, body: String) -> Any
  description "Publish a new page in a space. The page is created live and visible to everyone who can see the space, is indexed for search, and notifies the space's watchers. It is created at the top level of the space unless the space's own structure places it otherwise, with no labels and no restrictions. Returns the new page's id, title and links. The content is sent as Confluence storage format — XHTML-like markup, e.g. `<p>text</p>` — not Markdown and not wiki markup"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{site}.atlassian.net/wiki"
  url = fmt("{base}/api/v2/pages")
  content_type = "application/json"
  status = "current"
  body_representation = "storage"
  payload = { body: { representation: body_representation, value: body }, spaceId: space_id, status, title }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
