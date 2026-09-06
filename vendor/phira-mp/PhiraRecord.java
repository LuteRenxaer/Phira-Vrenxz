package top.rymc.phira.main.game.record;

import com.github.luben.zstd.Zstd;
import com.github.luben.zstd.ZstdInputStream;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.ByteBufUtil;
import io.netty.buffer.Unpooled;
import io.netty.handler.codec.CodecException;
import io.netty.handler.codec.CorruptedFrameException;
import io.netty.handler.codec.DecoderException;
import io.netty.util.ReferenceCountUtil;
import lombok.AccessLevel;
import lombok.Getter;
import lombok.RequiredArgsConstructor;
import lombok.Setter;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;
import top.rymc.phira.main.util.PhiraFetcher;
import top.rymc.phira.main.util.TimeUtil;
import top.rymc.phira.protocol.PacketRegistry;
import top.rymc.phira.protocol.codec.Encodeable;
import top.rymc.phira.protocol.data.monitor.judge.JudgeEvent;
import top.rymc.phira.protocol.data.monitor.touch.TouchFrame;
import top.rymc.phira.protocol.handler.server.ServerBoundPacketHandler;
import top.rymc.phira.protocol.handler.server.SimpleServerBoundPacketHandler;
import top.rymc.phira.protocol.packet.ClientBoundPacket;
import top.rymc.phira.protocol.packet.ServerBoundPacket;
import top.rymc.phira.protocol.packet.serverbound.ServerBoundJudgesPacket;
import top.rymc.phira.protocol.packet.serverbound.ServerBoundTouchesPacket;
import top.rymc.phira.protocol.util.NettyPacketUtil;
import top.rymc.phira.protocol.util.PacketWriter;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.file.Path;
import java.time.OffsetDateTime;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.function.Function;
import java.util.stream.Collectors;
import java.util.zip.Deflater;
import java.util.zip.InflaterInputStream;

@Getter
public final class PhiraRecord implements Encodeable {

    private static final Logger LOGGER = LogManager.getLogger("PhiraRecord");

    private static final FileHeader fileHeader = new FileHeader();
    private static final int formatVersion = 1;
    @Setter
    private static CompressionType formatCompressionType = CompressionType.ZSTD;

    private FileHeader.FileType fileType;
    private int fileVersion;
    private CompressionType fileCompressionType;

    private int id;
    private long time;
    private int chart;
    private String chartName;
    private int user;
    private String userName;
    private List<TouchFrame> touchFrames;
    private List<JudgeEvent> judgeEvents;

    private PhiraRecord() {
    }

    public PhiraRecord(int id, long time, int chart, String chartName, int user, String userName, List<TouchFrame> touchFrames, List<JudgeEvent> judgeEvents) {
        this.id = id;
        this.time = time;
        this.chart = chart;
        this.chartName = chartName;
        this.user = user;
        this.userName = userName;
        this.touchFrames = touchFrames;
        this.judgeEvents = judgeEvents;
    }

    @Override
    public void encode(ByteBuf byteBuf) {
        PacketWriter.write(byteBuf, fileHeader);
        PacketWriter.write(byteBuf, formatVersion);
        PacketWriter.write(byteBuf, formatCompressionType);

        ByteBuf tmpBuf = Unpooled.buffer();
        try {
            PacketWriter.write(tmpBuf, id);
            PacketWriter.write(tmpBuf, time);
            PacketWriter.write(tmpBuf, chart);
            PacketWriter.write(tmpBuf, chartName);
            PacketWriter.write(tmpBuf, user);
            PacketWriter.write(tmpBuf, userName);
            PacketWriter.write(tmpBuf, touchFrames);
            PacketWriter.write(tmpBuf, judgeEvents);

            byteBuf.writeBytes(formatCompressionType.compress(tmpBuf));
        } finally {
            ReferenceCountUtil.release(tmpBuf);
        }

    }

    public void decode(ByteBuf buf) {
        this.fileType = fileHeader.check(buf);
        if (fileType == FileHeader.FileType.Unknown) {
            throw new IllegalArgumentException("Unknown file type: file header does not match JPhiraRec or TPhiraRec format");
        }

        this.fileCompressionType = CompressionType.NONE;

        if (fileType == FileHeader.FileType.TPhiraRec) {
            try {
                decodeAsTPhiraRec(buf);
            } catch (IOException | CodecException e) {
                throw new RuntimeException("Failed to decode TPhiraRec format record", e);
            }
            return;
        }

        this.fileVersion = buf.readIntLE();

        if (fileVersion == 0) {
            try {
                decodeAsV0JPhiraRec(buf);
            } catch (IOException | CodecException e) {
                throw new RuntimeException("Failed to decode JPhiraRec version 0 format record", e);
            }
            return;
        }

        if (fileVersion == 1) {
            this.fileCompressionType = CompressionType.decode(buf);
            ByteBuf decompressed = fileCompressionType.decompress(buf);
            try {
                decodeAsV1JPhiraRec(decompressed);
            } finally {
                ReferenceCountUtil.safeRelease(decompressed);
            }
            buf.skipBytes(buf.readableBytes());
            return;
        }

        throw new IllegalArgumentException(String.format("Unsupported file version: %d (supported version: %d)", fileVersion, formatVersion));

    }

    private void decodeAsV1JPhiraRec(ByteBuf buf) {
        this.id = buf.readIntLE();
        this.time = buf.readLongLE();
        this.chart = buf.readIntLE();
        this.chartName = NettyPacketUtil.decodeString(buf,Short.MAX_VALUE);
        this.user = buf.readIntLE();
        this.userName = NettyPacketUtil.decodeString(buf,Short.MAX_VALUE);
        this.touchFrames = NettyPacketUtil.decodeList(buf, TouchFrame::decode);
        this.judgeEvents = NettyPacketUtil.decodeList(buf, JudgeEvent::decode);
    }

    private void decodeAsV0JPhiraRec(ByteBuf buf) throws IOException {
        this.id = buf.readIntLE();
        this.time = getTimeStamp(id);
        this.chart = buf.readIntLE();
        this.chartName = NettyPacketUtil.decodeString(buf,Short.MAX_VALUE);
        this.user = buf.readIntLE();
        this.userName = NettyPacketUtil.decodeString(buf,Short.MAX_VALUE);
        this.touchFrames = NettyPacketUtil.decodeList(buf, TouchFrame::decode);
        this.judgeEvents = NettyPacketUtil.decodeList(buf, JudgeEvent::decode);
    }

    private void decodeAsTPhiraRec(ByteBuf buf) throws IOException {
        this.chart = buf.readIntLE();
        this.chartName = PhiraFetcher.GET_CHART_INFO.apply(chart).getName();
        this.user = buf.readIntLE();
        this.userName = PhiraFetcher.GET_USER_INFO_BY_ID.apply(user).getName();
        this.id = buf.readIntLE();
        this.time = getTimeStamp(id);
        this.fileVersion = 0;

        this.touchFrames = new ArrayList<>();
        this.judgeEvents = new ArrayList<>();

        ServerBoundPacketHandler dataProcessor = new SimpleServerBoundPacketHandler() {

            @Override
            protected void sendPacket(ClientBoundPacket packet) {

            }

            @Override
            public void handle(ServerBoundTouchesPacket packet) {
                touchFrames.addAll(packet.getFrames());
            }

            @Override
            public void handle(ServerBoundJudgesPacket packet) {
                judgeEvents.addAll(packet.getJudges());
            }
        };

        while (buf.readableBytes() > 0) {
            int length = buf.readIntLE();
            if (length < 0) {
                throw new CorruptedFrameException(
                    String.format("Invalid packet length: %d (negative value)", length));
            }
            if (buf.readableBytes() < length) {
                throw new CorruptedFrameException(
                    String.format("Insufficient data for packet: required %d bytes, but only %d bytes available",
                        length, buf.readableBytes()));
            }

            ByteBuf packetBuf = buf.readRetainedSlice(length);
            try {
                ServerBoundPacket packet = PacketRegistry.ServerBound.decode(packetBuf);
                packet.handle(dataProcessor);
            } finally {
                ReferenceCountUtil.safeRelease(packetBuf);
            }
        }
    }

    private static long getTimeStamp(int recordId) {
        try {
            return PhiraFetcher.GET_RECORD_INFO.apply(recordId).getTime().toInstant().toEpochMilli();
        } catch (IOException e) {
            LOGGER.warn("Failed get record info: {}, using system time", recordId);
            return System.currentTimeMillis();
        }

    }


    public static boolean saveAsFile(PhiraRecord record, Path path) {
        File file = path.resolve(String.format("%d.phirarec", record.id)).toFile();
        File parent = file.getParentFile();
        if (parent != null && !parent.exists()) {
            parent.mkdirs();
        }

        boolean success = PhiraRecordIO.saveRecordToFile(record, file);
        if (!success) {
            LOGGER.error("Failed to save record to file: id={}, path={}", record.id, file.getAbsolutePath());
        }
        return success;
    }

    public static List<PhiraRecord> readFromDirectory(Path dirPath) {
        File dir = dirPath.toFile();
        if (!dir.exists()) {
            dir.mkdirs();
        }

        if (!dir.isDirectory()) {
            LOGGER.warn("Path is not a directory: {}", dir.getAbsolutePath());
            return new ArrayList<>();
        }

        File[] files = dir.listFiles((d, name) -> name.toLowerCase().endsWith(".phirarec"));
        if (files == null) {
            return new ArrayList<>();
        }

        return Arrays.stream(files)
                .map(PhiraRecordIO::readRecordFromFile)
                .filter(Objects::nonNull)
                .toList();
    }

    public static PhiraRecord readFromFile(Path path) {
        File file = path.toFile();
        if (!file.exists()) {
            return null;
        }

        if (!file.isFile()) {
            return null;
        }

        return PhiraRecordIO.readRecordFromFile(file);
    }

    private static class PhiraRecordIO {
        private static boolean saveRecordToFile(PhiraRecord record, File file) {
            ByteBuf byteBuf = Unpooled.buffer();

            try (FileOutputStream fos = new FileOutputStream(file);
                 FileChannel channel = fos.getChannel()) {

                record.encode(byteBuf);

                ByteBuffer buffer = byteBuf.nioBuffer();
                channel.write(buffer);
                return true;
            } catch (Exception e) {
                LOGGER.error("Failed to save record to file: {} - {}", file.getAbsolutePath(), e.getMessage(), e);
                return false;
            } finally {
                byteBuf.release();
            }
        }

        private static PhiraRecord readRecordFromFile(File file) {
            System.out.println("reading: " + file.getName());
            PhiraRecord record = new PhiraRecord();

            try (FileInputStream fis = new FileInputStream(file);
                 FileChannel channel = fis.getChannel()) {

                long fileSize = channel.size();
                if (fileSize > Integer.MAX_VALUE) {
                    LOGGER.error("Record file too large: {} ({} bytes)", file.getAbsolutePath(), fileSize);
                    return null;
                }

                ByteBuffer buffer = ByteBuffer.allocate((int) fileSize);
                channel.read(buffer);
                buffer.flip();

                ByteBuf byteBuf = Unpooled.wrappedBuffer(buffer);
                try {
                    record.decode(byteBuf);
                    return record;
                } finally {
                    byteBuf.release();
                }
            } catch (IllegalArgumentException e) {
                LOGGER.error("Invalid record file format: {} - {}", file.getAbsolutePath(), e.getMessage());
                return null;
            } catch (Exception e) {
                LOGGER.error("Failed to read record from file: {} - {}", file.getAbsolutePath(), e.getMessage(), e);
                return null;
            }
        }
    }

    private static final class FileHeader implements Encodeable {

        private static final byte[] fileHeader = new byte[] { 'P', 'H', 'I', 'R', 'A', 'R', 'E', 'C' };

        @Override
        public void encode(ByteBuf buf) {
            buf.writeBytes(fileHeader);
        }

        public FileType check(ByteBuf buf) {

            short head = buf.getShort(buf.readerIndex());
            if (head == 0x504d || head == 0x4d50) {
                buf.readShort();
                return FileType.TPhiraRec;
            }


            byte[] header = new byte[fileHeader.length];
            buf.readBytes(header);
            for (int i = 0; i < fileHeader.length; i++) {
                if (header[i] != fileHeader[i]) {
                    return FileType.Unknown;
                }
            }

            return FileType.JPhiraRec;
        }

        @Getter
        @RequiredArgsConstructor
        private enum FileType {
            JPhiraRec,
            TPhiraRec,
            Unknown
        }
    }

    @RequiredArgsConstructor
    @Getter(AccessLevel.PRIVATE)
    public enum CompressionType implements Encodeable {
        NONE(0x00, ByteBuf::retainedSlice, ByteBuf::retainedSlice),
        ZSTD(0x01,
            (input) -> {
                ByteArrayInputStream rawIn = new ByteArrayInputStream(ByteBufUtil.getBytes(input));
                try (ZstdInputStream in = new ZstdInputStream(rawIn)) {
                    ByteArrayOutputStream out = new ByteArrayOutputStream();

                    byte[] buffer = new byte[4096];
                    int index;

                    while ((index = in.read(buffer)) != -1) {
                        out.write(buffer, 0, index);
                    }

                    return Unpooled.wrappedBuffer(out.toByteArray());

                } catch (IOException e) {
                    throw new DecoderException("Zstd decompression failed", e);
                }
            },
            (input) -> {
                byte[] src = ByteBufUtil.getBytes(input);
                byte[] compressed = Zstd.compress(src, Zstd.defaultCompressionLevel());
                return Unpooled.wrappedBuffer(compressed);

            }),
        DEFLATE(0x02,
            (input) -> {
                ByteArrayInputStream rawIn = new ByteArrayInputStream(ByteBufUtil.getBytes(input));
                try (InflaterInputStream in = new InflaterInputStream(rawIn)) {

                    ByteArrayOutputStream out = new ByteArrayOutputStream();

                    byte[] buf = new byte[4096];
                    int n;

                    while ((n = in.read(buf)) != -1) {
                        out.write(buf, 0, n);
                    }

                    return Unpooled.wrappedBuffer(out.toByteArray());

                } catch (IOException e) {
                    throw new DecoderException("Deflate decompress failed", e);
                }
        },
        (input) -> {
            byte[] src = ByteBufUtil.getBytes(input);

            Deflater deflater = new Deflater();
            deflater.setInput(src);
            deflater.finish();

            byte[] buf = new byte[4096];
            ByteArrayOutputStream out = new ByteArrayOutputStream();

            while (!deflater.finished()) {
                int index = deflater.deflate(buf);
                out.write(buf, 0, index);
            }

            deflater.end();

            return Unpooled.wrappedBuffer(out.toByteArray());

        });

        private final int id;
        private final Function<ByteBuf, ByteBuf> decompressor;
        private final Function<ByteBuf, ByteBuf> compressor;

        public ByteBuf decompress(ByteBuf buf) {
            return decompressor.apply(buf);
        }

        public ByteBuf compress(ByteBuf buf) {
            return compressor.apply(buf);
        }

        private static Map<Integer, CompressionType> getCompressionTypeMap() {
            return Map.copyOf(Arrays.stream(values()).collect(Collectors.toMap(
                    CompressionType::getId,
                    Function.identity()
            )));
        }

        private static final Map<Integer, CompressionType> COMPRESSION_TYPE_MAP = getCompressionTypeMap();

        public static CompressionType decode(ByteBuf buf) {
            int id = buf.readByte();
            CompressionType type = COMPRESSION_TYPE_MAP.get(id);
            if (type == null) {
                throw new DecoderException("Unknown CompressionType id: " + id);
            }
            return type;
        }

        @Override
        public void encode(ByteBuf buf) {
            buf.writeByte(id);
        }
    }
}
