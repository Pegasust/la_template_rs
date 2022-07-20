##// #define COMMAND "win_command"
##// #define BECOME
- block:  
##// Create a new vm if vm is not yet created
  - name: Launch new multipass instances
    COMMAND: multipass launch
      --cpus "{{ item.vcpu }}"
      --disk "{{ item.disk }}"
      --mem "{{ item.mem }}"
      --name "{{ vm_names[item.static_ip] }}"
      {% if item.cloud_init is defined %}
      --cloud-init "{{ ansible_env.TEMP|default('.') }}/{{ item.cloud_init|trim }}-{{ vm_names[item.static_ip] }}"
      {% endif %}
      {% for net in item.networks %}
      --network name="{{ net.name|trim }}",mode="{{ net.mode|trim }}"
      {% endfor %}
    register: mp_launch
#//  changed_when: todo
    failed_when: mp_launch.rc != 0
#//  active: not yet existed
#//  ignore: queried state is deleted
    when:
    - not ansible_check_mode
    - current_state[item.static_ip] == 'not_exist'
    - item.state|lower != 'deleted'
    loop: "{{ multipass_vm_instances }}"

#// Recover a vm if it's found to be deleted
  - name: Recover deleted multipass instances
    COMMAND: multipass recover "{{ vm_names[item.static_ip] }}"
    register: mp_recover
#//  changed_when: todo
    failed_when: mp_recover.rc != 0
    when:
    - not ansible_check_mode
    - current_state[item.static_ip] == 'deleted'
    - item.state|lower != 'deleted'
    loop: "{{ multipass_vm_instances }}"

#// For now, can't change the machine configuration
#// imagine each instance follows a "template"
#// representing a unit of horizontal scale
#// TODO: use `multipass get <node>.{cpus,disk,mem}`
#// and identify diffs. Sounds complicated enough
#// for a module

#// Now, assume that the VMs are properly configured

  - name: Ensure vms running for non-running & stopped->suspended
    COMMAND: multipass start "{{ vm_names[item.static_ip] }}"
    register: mp_start
#//  changed_when: todo
    failed_when: mp_start.rc != 0
    when:
    - not ansible_check_mode
    - (item.state|lower == 'running' and 
      current_state[item.static_ip] != 'running') or
      (item.state|lower == 'suspended' and
      current_state[item.static_ip] == 'stopped')

    loop: "{{ multipass_vm_instances }}"

#//  TODO: tricky: what if the vm is initially stopped?
#//  Idea: inject multipass start above
#//  then we will suspend at this part
#//  TODO: check that this trick works well
#//  multipass suspend: 
#//  - stopped  ->0
#//  - running  ->0
#//  - suspended->0
  - name: Ensure vms suspended for non-suspended
    COMMAND: multipass suspend "{{ vm_names[item.static_ip] }}"
    register: mp_suspend
    changed_when: true # since the when condition goes thru, guaranteed change.
    failed_when: mp_suspend.rc != 0
    when: 
    - not ansible_check_mode
    - item.state|lower == "suspended"
    - current_state[item.static_ip] is not in (['suspended','stopped']|uniq)
    loop: "{{ multipass_vm_instances }}"

  - name: Ensure vms stopped for non-stopped
    COMMAND: multipass stop "{{ vm_names[item.static_ip] }}"
    register: mp_stop
    changed_when: true
    failed_when: mp_stop.rc != 0
    when: 
    - not ansible_check_mode
    - item.state|lower == "stopped"
    - current_state[item.static_ip] != 'stopped'
    loop: "{{ multipass_vm_instances }}"

  - name: Delete vms that declare to be 'deleted'
    COMMAND: multipass delete "{{ vm_names[item.static_ip] }}"
    register: mp_delete
    changed_when: true
    failed_when: mp_delete.rc != 0
    when:
    - not ansible_check_mode
    - item.state|lower == "deleted"
    - current_state[item.static_ip] is not in (["deleted", "not_exist"]|unique)
    loop: "{{ multipass_vm_instances }}"
  BECOME
