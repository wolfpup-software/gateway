# generate_tls.sh
# brian taylor vann
#
# args ($1: destination) ($2: config_filepath)


curr_dir=`dirname $0`
target_key=$curr_dir/resources/self-signed-key.key
target_cert=$curr_dir/resources/self-signed-cert.crt
subject="/C=US/ST=CA/L=SF/O=Toshokan/OU=Education/CN=*.toshokan.com/emailAddress=brian@toshokan.com"
# subject_alt_name="subjectAltName=DNS:*.toshokan.org,IP:0.0.0.0,IP:127.0.0.1"

openssl req -new -newkey rsa:4096 -x509 -sha256 \
    -days 365 -nodes \
    -keyout $target_key \
    -subj $subject \
    -out $target_cert
