# generate_tls.sh
# brian taylor vann
#
# args ($1: destination) ($2: config_filepath)


curr_dir=`dirname $0`
target_key=$curr_dir/demo/self-signed-key.key
target_cert=$curr_dir/demo/self-signed-cert.crt
subject="/C=US/ST=CA/L=SF/O=Toshokan/OU=Education/CN=*.toshokan.org/emailAddress=taylor@toshokan.org"

openssl req -new -newkey rsa:2048 -x509 -sha256 \
    -days 365 -nodes \
    -keyout $target_key \
    -subj $subject \
    -out $target_cert
