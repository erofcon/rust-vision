<?xml version="1.0" encoding="utf-8" ?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
    <xsl:template match="/">
        <html>
            <head>
                <title>RTMP Statistics</title>
                <style>
                    body {
                    font-family: Arial, sans-serif;
                    margin: 20px;
                    background-color: #f5f5f5;
                    }
                    .container {
                    background: white;
                    padding: 20px;
                    border-radius: 10px;
                    box-shadow: 0 2px 10px rgba(0,0,0,0.1);
                    }
                    h1 {
                    color: #333;
                    text-align: center;
                    }
                    table {
                    width: 100%;
                    border-collapse: collapse;
                    margin: 20px 0;
                    }
                    th, td {
                    border: 1px solid #ddd;
                    padding: 8px;
                    text-align: left;
                    }
                    th {
                    background-color: #f2f2f2;
                    font-weight: bold;
                    }
                    tr:nth-child(even) {
                    background-color: #f9f9f9;
                    }
                    .server-info {
                    background: #e7f3ff;
                    border-left: 4px solid #2196f3;
                    padding: 15px;
                    margin: 20px 0;
                    }
                    .stream-info {
                    background: #f0f8ff;
                    border-left: 4px solid #4caf50;
                    padding: 15px;
                    margin: 20px 0;
                    }
                    .client-info {
                    background: #fff3e0;
                    border-left: 4px solid #ff9800;
                    padding: 15px;
                    margin: 20px 0;
                    }
                </style>
            </head>
            <body>
                <div class="container">
                    <h1>RTMP Server Statistics</h1>

                    <div class="server-info">
                        <h2>Server Information</h2>
                        <p><strong>Nginx Version:</strong> <xsl:value-of select="rtmp/nginx_version"/></p>
                        <p><strong>Nginx RTMP Version:</strong> <xsl:value-of select="rtmp/nginx_rtmp_version"/></p>
                        <p><strong>Compiler:</strong> <xsl:value-of select="rtmp/compiler"/></p>
                        <p><strong>Built:</strong> <xsl:value-of select="rtmp/built"/></p>
                        <p><strong>PID:</strong> <xsl:value-of select="rtmp/pid"/></p>
                        <p><strong>Uptime:</strong> <xsl:value-of select="rtmp/uptime"/></p>
                        <p><strong>Accepted:</strong> <xsl:value-of select="rtmp/naccepted"/></p>
                        <p><strong>Bytes In:</strong> <xsl:value-of select="rtmp/bytes_in"/></p>
                        <p><strong>Bytes Out:</strong> <xsl:value-of select="rtmp/bytes_out"/></p>
                    </div>

                    <xsl:for-each select="rtmp/server">
                        <div class="server-info">
                            <h2>Server <xsl:value-of select="port"/></h2>

                            <xsl:for-each select="application">
                                <div class="stream-info">
                                    <h3>Application: <xsl:value-of select="name"/></h3>

                                    <xsl:if test="live">
                                        <h4>Live Streams</h4>
                                        <table>
                                            <tr>
                                                <th>Stream Name</th>
                                                <th>Time</th>
                                                <th>Bytes In</th>
                                                <th>Bytes Out</th>
                                                <th>BW In</th>
                                                <th>BW Out</th>
                                                <th>Clients</th>
                                            </tr>
                                            <xsl:for-each select="live/stream">
                                                <tr>
                                                    <td><xsl:value-of select="name"/></td>
                                                    <td><xsl:value-of select="time"/></td>
                                                    <td><xsl:value-of select="bytes_in"/></td>
                                                    <td><xsl:value-of select="bytes_out"/></td>
                                                    <td><xsl:value-of select="bw_in"/></td>
                                                    <td><xsl:value-of select="bw_out"/></td>
                                                    <td><xsl:value-of select="nclients"/></td>
                                                </tr>
                                            </xsl:for-each>
                                        </table>

                                        <xsl:for-each select="live/stream">
                                            <xsl:if test="client">
                                                <div class="client-info">
                                                    <h4>Clients for stream: <xsl:value-of select="name"/></h4>
                                                    <table>
                                                        <tr>
                                                            <th>ID</th>
                                                            <th>Address</th>
                                                            <th>Time</th>
                                                            <th>Flashver</th>
                                                            <th>Dropped</th>
                                                            <th>Timestamp</th>
                                                        </tr>
                                                        <xsl:for-each select="client">
                                                            <tr>
                                                                <td><xsl:value-of select="id"/></td>
                                                                <td><xsl:value-of select="address"/></td>
                                                                <td><xsl:value-of select="time"/></td>
                                                                <td><xsl:value-of select="flashver"/></td>
                                                                <td><xsl:value-of select="dropped"/></td>
                                                                <td><xsl:value-of select="timestamp"/></td>
                                                            </tr>
                                                        </xsl:for-each>
                                                    </table>
                                                </div>
                                            </xsl:if>
                                        </xsl:for-each>
                                    </xsl:if>
                                </div>
                            </xsl:for-each>
                        </div>
                    </xsl:for-each>

                    <div style="text-align: center; margin-top: 20px; color: #666;">
                        <p>Generated at: <script>document.write(new Date().toLocaleString());</script></p>
                        <p><a href="/">← Back to Player</a></p>
                    </div>
                </div>
            </body>
        </html>
    </xsl:template>
</xsl:stylesheet>