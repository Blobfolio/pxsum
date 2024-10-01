_basher___pxsum() {
	local cur prev opts
	COMPREPLY=()
	cur="${COMP_WORDS[COMP_CWORD]}"
	prev="${COMP_WORDS[COMP_CWORD-1]}"
	opts=()
	[[ " ${COMP_LINE} " =~ " --bench " ]] || opts+=("--bench")
	if [[ ! " ${COMP_LINE} " =~ " -c " ]] && [[ ! " ${COMP_LINE} " =~ " --check " ]]; then
		opts+=("-c")
		opts+=("--check")
	fi
	if [[ ! " ${COMP_LINE} " =~ " -g " ]] && [[ ! " ${COMP_LINE} " =~ " --group-by-checksum " ]]; then
		opts+=("-g")
		opts+=("--group-by-checksum")
	fi
	if [[ ! " ${COMP_LINE} " =~ " -h " ]] && [[ ! " ${COMP_LINE} " =~ " --help " ]]; then
		opts+=("-h")
		opts+=("--help")
	fi
	[[ " ${COMP_LINE} " =~ " --no-warnings " ]] || opts+=("--no-warnings")
	[[ " ${COMP_LINE} " =~ " --only-dupes " ]] || opts+=("--only-dupes")
	if [[ ! " ${COMP_LINE} " =~ " -q " ]] && [[ ! " ${COMP_LINE} " =~ " --quiet " ]]; then
		opts+=("-q")
		opts+=("--quiet")
	fi
	[[ " ${COMP_LINE} " =~ " --strict " ]] || opts+=("--strict")
	if [[ ! " ${COMP_LINE} " =~ " -V " ]] && [[ ! " ${COMP_LINE} " =~ " --version " ]]; then
		opts+=("-V")
		opts+=("--version")
	fi
	opts+=("-d")
	opts+=("--dir")
	[[ " ${COMP_LINE} " =~ " -j " ]] || opts+=("-j")
	opts=" ${opts[@]} "
	if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
		COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
		return 0
	fi
	case "${prev}" in
		-d|--dir)
			if [ -z "$( declare -f _filedir )" ]; then
				COMPREPLY=( $( compgen -f "${cur}" ) )
			else
				COMPREPLY=( $( _filedir ) )
			fi
			return 0
			;;
		*)
			COMPREPLY=()
			;;
	esac
	COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
	return 0
}
complete -F _basher___pxsum -o bashdefault -o default pxsum
