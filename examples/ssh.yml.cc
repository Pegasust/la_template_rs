# // Defines:
# // COMMAND="command" or "win_command"

#ifndef COMMAND
#error COMMAND expected to be defined
#endif

#ifndef BECOME
#error BECOME expected to be defined
#endif

#// See this hack: https://stackoverflow.com/questions/2751870/how-exactly-does-the-double-stringize-trick-work
#define _STR(x) #x
#define STR(x) _STR(x)

#ifndef SSH_KEY_PRIVATE
#define SSH_KEY_PRIVATE(POST) ~/.ssh/{{ ssh_key_filename }}POST
#define SSH_KEY_PRIVATE_IN(POST) "~/.ssh/"+ssh_key_filename+STR(POST)
#endif

#ifndef TEMP_FOLDER
#define TEMP_FOLDER(PRE, POST) PRE ## {{ ansible_env.TEMP|default('.') }}POST
#endif

- block:
  - name: Generate ssh key if not presented yet
    openssh_keypair:
      path: STR(SSH_KEY_PRIVATE())
      type: rsa
      size: 4096
      state: present
      force: no
    delegate_to: 127.0.0.1
    delegate_facts: true 
  - name: Put public key into fact
    set_fact: ssh_host_pubkey={{ lookup("file", SSH_KEY_PRIVATE_IN(.pub)) }}
  - name: ssh_host_pubkey
    debug:
      var: ssh_host_pubkey

- block:
  - name: Append to authorized_keys
    COMMAND: multipass exec {{ vm_names[item.static_ip] }} -- bash -c "echo {{ ssh_host_pubkey | default(hostvars['127.0.0.1']['ansible_facts']['ssh_host_pubkey']) }} >>~/.ssh/authorized_keys"
    register: append_authorized_keys
    failed_when: append_authorized_keys.rc != 0
    changed_when: true
    when: item.lan_reachable
    loop: "{{ multipass_vm_instances }}"
  - name: Back up original authorized_keys
    COMMAND: multipass exec {{ vm_names[item.static_ip] }} -- bash -c "cp ~/.ssh/authorized_keys ~/.ssh/.authorized_keys.bck"
    register: backup
    changed_when: true
    when: item.lan_reachable
    loop: "{{ multipass_vm_instances  }}"
  - name: Make authorized_keys unique only
    COMMAND: multipass exec {{ vm_names[item.static_ip] }} -- bash -c "cat ~/.ssh/authorized_keys | sort | uniq | tee ~/.ssh/authorized_keys"
    register: uniq_authorized_keys
    failed_when: uniq_authorized_keys.rc != 0 or unique_authorized_keys.stdout | length <=1 
    changed_when: true
    when: item.lan_reachable
    loop: "{{ multipass_vm_instances }}"
  
  BECOME
