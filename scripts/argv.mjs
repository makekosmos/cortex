export function flag(args, name) {
  const index = args.indexOf(name);
  return index < 0 ? null : args[index + 1];
}
