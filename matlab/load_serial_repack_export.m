function capture = load_serial_repack_export(config_path, export_dir)
%LOAD_SERIAL_REPACK_EXPORT Load serial-repack channel data exported as bin files.
%
%   capture = load_serial_repack_export(config_path, export_dir)
%
% Inputs:
%   config_path - original serial-repack TOML config used for recording
%   export_dir  - directory created by `serial-repack export`
%
% Output:
%   capture.<channel>.data          uint8(packet_len, packet_count)
%   capture.<channel>.timestamps_ns uint64(packet_count, 1)
%   capture.<channel>.timestamps_s  double(packet_count, 1), relative to first packet

channels = parse_serial_repack_config(config_path);
capture = struct();
capture.first_timestamp_ns = uint64(0);

first_set = false;
for i = 1:numel(channels)
    ch = channels(i);
    if ~ch.enabled
        continue;
    end

    channel_dir = fullfile(export_dir, ch.name);
    data_path = fullfile(channel_dir, 'data.bin');
    timestamp_path = fullfile(channel_dir, 'timestamps_ns.bin');

    timestamps_ns = read_uint64_file(timestamp_path);
    packet_count = numel(timestamps_ns);
    data = read_packet_file(data_path, ch.packet_len, packet_count);

    if packet_count > 0
        if ~first_set || timestamps_ns(1) < capture.first_timestamp_ns
            capture.first_timestamp_ns = timestamps_ns(1);
            first_set = true;
        end
    end

    field = matlab.lang.makeValidName(ch.name);
    capture.(field).name = ch.name;
    capture.(field).packet_len = ch.packet_len;
    capture.(field).data = data;
    capture.(field).timestamps_ns = timestamps_ns;
end

fields = fieldnames(capture);
for i = 1:numel(fields)
    field = fields{i};
    if strcmp(field, 'first_timestamp_ns')
        continue;
    end
    timestamps_ns = capture.(field).timestamps_ns;
    capture.(field).timestamps_s = double(timestamps_ns - capture.first_timestamp_ns) ./ 1e9;
end
end

function timestamps_ns = read_uint64_file(path)
fid = fopen(path, 'rb', 'ieee-le');
assert(fid >= 0, 'failed to open %s', path);
cleanup = onCleanup(@() fclose(fid));
timestamps_ns = fread(fid, inf, '*uint64');
end

function data = read_packet_file(path, packet_len, packet_count)
fid = fopen(path, 'rb', 'ieee-le');
assert(fid >= 0, 'failed to open %s', path);
cleanup = onCleanup(@() fclose(fid));
data = fread(fid, [packet_len, packet_count], '*uint8');
end

function channels = parse_serial_repack_config(path)
text = fileread(path);
lines = regexp(text, '\r?\n', 'split');
channels = struct('name', {}, 'enabled', {}, 'packet_len', {});
idx = 0;
section = '';

for i = 1:numel(lines)
    line = strip_comment(strtrim(lines{i}));
    if isempty(line)
        continue;
    end

    if strcmp(line, '[[channels]]')
        idx = idx + 1;
        channels(idx).name = '';
        channels(idx).enabled = true;
        channels(idx).packet_len = 0;
        section = 'channel';
        continue;
    elseif strcmp(line, '[channels.packet]')
        section = 'packet';
        continue;
    elseif startsWith(line, '[')
        section = '';
        continue;
    end

    if idx == 0 || ~contains(line, '=')
        continue;
    end

    eq = strfind(line, '=');
    key = strtrim(line(1:eq(1) - 1));
    value = strtrim(line(eq(1) + 1:end));

    if strcmp(section, 'channel')
        if strcmp(key, 'name')
            channels(idx).name = strip_quotes(value);
        elseif strcmp(key, 'enabled')
            channels(idx).enabled = strcmpi(value, 'true');
        end
    elseif strcmp(section, 'packet')
        if strcmp(key, 'packet_len')
            channels(idx).packet_len = str2double(value);
        end
    end
end

for i = 1:numel(channels)
    assert(~isempty(channels(i).name), 'channel %d missing name', i);
    assert(channels(i).packet_len > 0, 'channel %s missing packet_len', channels(i).name);
end
end

function line = strip_comment(line)
comment_pos = strfind(line, '#');
if ~isempty(comment_pos)
    line = strtrim(line(1:comment_pos(1) - 1));
end
end

function value = strip_quotes(value)
value = strtrim(value);
if numel(value) >= 2 && value(1) == '"' && value(end) == '"'
    value = value(2:end - 1);
end
end
