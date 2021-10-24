resource "aws_s3_bucket" "card-counter" {
  bucket = <bucket-name>
  acl    = "public-read"
  policy = file("policy.json")

  website {
    index_document = "index.html"
    error_document = "index.html"
  }
}
